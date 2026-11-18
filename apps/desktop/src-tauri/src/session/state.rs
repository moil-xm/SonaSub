use std::{
    collections::VecDeque,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::Emitter;

use crate::{
    config::AppConfig,
    pipeline::metrics::SessionMetrics,
    subtitles::{SubtitleBatch, SubtitlePatch, SubtitleStore},
};

use super::{
    manager::ManagerInner,
    types::{SessionNotice, SessionSnapshot, SessionStatus, StatsEvent, StatusEvent},
};

const NOTICE_LIMIT: usize = 100;

pub(super) struct RuntimeState {
    pub(super) status: SessionStatus,
    pub(super) session_id: Option<String>,
    pub(super) store: SubtitleStore,
    pub(super) metrics: SessionMetrics,
    pub(super) notices: VecDeque<SessionNotice>,
    pub(super) config: AppConfig,
    /// Monotonic counter of every published state change.
    revision: u64,
    /// Revision of the current status.
    status_revision: u64,
    /// Revision of the current subtitle list.
    subtitle_revision: u64,
    /// Revision of the current statistics.
    stats_revision: u64,
}

impl RuntimeState {
    pub(super) fn new(config: AppConfig, load_error: Option<&str>) -> Self {
        let mut notices = VecDeque::new();
        if let Some(error) = load_error {
            notices.push_back(SessionNotice {
                level: "error".to_owned(),
                message: error.to_owned(),
                at_ms: now_ms(),
            });
        }
        Self {
            status: SessionStatus::Idle,
            session_id: None,
            store: SubtitleStore::new(String::new(), config.ui.history_limit),
            metrics: SessionMetrics::new(now_ms()),
            notices,
            config,
            revision: 0,
            status_revision: 0,
            subtitle_revision: 0,
            stats_revision: 0,
        }
    }

    pub(super) fn set_status(&mut self, status: SessionStatus) -> StatusEvent {
        self.status = status.clone();
        self.revision = self.revision.saturating_add(1);
        self.status_revision = self.revision;
        StatusEvent {
            revision: self.revision,
            status,
        }
    }
}

pub(super) fn finish_session(inner: &ManagerInner, failure: Option<&str>) {
    if let Some(error) = failure {
        publish_failed(inner, error);
    } else {
        publish_stopped(inner, Some("Session stopped."));
    }
}

pub(super) fn publish_stopped(inner: &ManagerInner, notice: Option<&str>) {
    publish_status(inner, SessionStatus::Idle);
    if let Some(notice) = notice {
        add_notice(inner, "info", notice.to_owned());
    }
}

pub(super) fn publish_failed(inner: &ManagerInner, error: &str) {
    publish_status(
        inner,
        SessionStatus::Failed {
            error: error.to_owned(),
        },
    );
    add_notice(inner, "error", error.to_owned());
}

pub(super) fn publish_status(inner: &ManagerInner, status: SessionStatus) {
    let event = inner.runtime().set_status(status);
    let _ = inner.app.emit("session://status", event);
}

pub(super) fn publish_subtitles(
    inner: &ManagerInner,
    session_id: &str,
    patches: Vec<SubtitlePatch>,
) {
    let revision = {
        let mut runtime = inner.runtime();
        runtime.revision = runtime.revision.saturating_add(1);
        runtime.subtitle_revision = runtime.revision;
        runtime.revision
    };
    let batch = SubtitleBatch {
        revision,
        session_id: session_id.to_owned(),
        patches,
    };
    let _ = inner.app.emit("session://subtitles", batch);
}

pub(super) fn publish_stats(inner: &ManagerInner) {
    let event = {
        let mut runtime = inner.runtime();
        runtime.revision = runtime.revision.saturating_add(1);
        runtime.stats_revision = runtime.revision;
        StatsEvent {
            revision: runtime.revision,
            stats: runtime.metrics.snapshot(),
        }
    };
    let _ = inner.app.emit("session://stats", event);
}

pub(super) fn add_notice(inner: &ManagerInner, level: &str, message: String) {
    let notice = SessionNotice {
        level: level.to_owned(),
        message,
        at_ms: now_ms(),
    };
    let emit = {
        let mut runtime = inner.runtime();
        let duplicate = runtime.notices.back().is_some_and(|last| {
            last.level == notice.level
                && last.message == notice.message
                && notice.at_ms.saturating_sub(last.at_ms) < 2_000
        });
        if duplicate {
            false
        } else {
            runtime.notices.push_back(notice.clone());
            while runtime.notices.len() > NOTICE_LIMIT {
                runtime.notices.pop_front();
            }
            true
        }
    };
    if emit {
        let _ = inner.app.emit("session://notice", notice);
    }
}

pub(super) fn snapshot(inner: &ManagerInner) -> SessionSnapshot {
    let runtime = inner.runtime();
    SessionSnapshot {
        revision: runtime.revision,
        session_id: runtime.session_id.clone(),
        status: runtime.status.clone(),
        status_revision: runtime.status_revision,
        subtitle_revision: runtime.subtitle_revision,
        stats_revision: runtime.stats_revision,
        segments: runtime.store.snapshot(),
        stats: runtime.metrics.snapshot(),
        notices: runtime.notices.iter().cloned().collect(),
        history_limit: runtime.store.history_limit(),
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
