use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::{
    config::AppConfig,
    speaker::SpeakerInfo,
    subtitles::{SubtitleSegment, TranslationState},
};

use super::types::{
    RecordSource, RecordState, RecordSubtitlePage, SourceRunState, StoredRecord, SubtitleCursor,
};

pub struct RecordStore {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl RecordStore {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create the record database directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let connection = Connection::open(&path).map_err(|error| {
            format!(
                "Failed to open the record database {}: {error}",
                path.display()
            )
        })?;
        connection
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS records (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    config_json TEXT NOT NULL,
                    state TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    started_at_ms INTEGER,
                    ended_at_ms INTEGER,
                    last_error TEXT
                );
                CREATE TABLE IF NOT EXISTS source_runs (
                    id TEXT PRIMARY KEY,
                    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
                    device_id TEXT NOT NULL,
                    device_name TEXT NOT NULL,
                    elapsed_base_ms INTEGER NOT NULL DEFAULT 0,
                    elapsed_ms INTEGER NOT NULL DEFAULT 0,
                    started_at_ms INTEGER NOT NULL,
                    ended_at_ms INTEGER,
                    state TEXT NOT NULL,
                    error TEXT
                );
                CREATE INDEX IF NOT EXISTS source_runs_record
                    ON source_runs(record_id, started_at_ms);
                CREATE TABLE IF NOT EXISTS subtitles (
                    id TEXT PRIMARY KEY,
                    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
                    source_run_id TEXT NOT NULL,
                    device_id TEXT NOT NULL,
                    device_name TEXT NOT NULL,
                    local_id INTEGER NOT NULL,
                    device_time_secs REAL NOT NULL,
                    start_secs REAL NOT NULL,
                    end_secs REAL,
                    occurrence_ms INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    is_final INTEGER NOT NULL,
                    speaker_json TEXT,
                    speaker_revision INTEGER NOT NULL DEFAULT 0,
                    translation_json TEXT,
                    source_language TEXT,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    UNIQUE(source_run_id, local_id)
                );
                CREATE INDEX IF NOT EXISTS subtitles_record_order
                    ON subtitles(record_id, occurrence_ms, id);
                ",
            )
            .map_err(|error| format!("Failed to initialize the record database: {error}"))?;
        Ok(Self {
            path,
            connection: Mutex::new(connection),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn recover_active_records(&self, now_ms: u64) -> Result<(), String> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE records
                 SET state = ?1, updated_at_ms = ?2
                 WHERE state IN ('starting', 'running', 'waitingForDevice')",
                params![RecordState::Paused.as_str(), now_ms as i64],
            )
            .map_err(|error| format!("Failed to recover interrupted records: {error}"))?;
        connection
            .execute(
                "UPDATE source_runs
                 SET state = ?1, ended_at_ms = ?2
                 WHERE state IN ('starting', 'running')",
                params![SourceRunState::Disconnected.as_str(), now_ms as i64],
            )
            .map_err(|error| format!("Failed to recover interrupted source runs: {error}"))?;
        Ok(())
    }

    pub fn save_record(&self, record: &StoredRecord) -> Result<(), String> {
        let config_json = serde_json::to_string(&record.config)
            .map_err(|error| format!("Failed to serialize the record configuration: {error}"))?;
        let connection = self.connection();
        connection
            .execute(
                "INSERT INTO records (
                    id, name, config_json, state, created_at_ms, updated_at_ms,
                    started_at_ms, ended_at_ms, last_error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    config_json = excluded.config_json,
                    state = excluded.state,
                    updated_at_ms = excluded.updated_at_ms,
                    started_at_ms = excluded.started_at_ms,
                    ended_at_ms = excluded.ended_at_ms,
                    last_error = excluded.last_error",
                params![
                    record.summary.id,
                    record.summary.name,
                    config_json,
                    record.summary.state.as_str(),
                    record.summary.created_at_ms as i64,
                    record.summary.updated_at_ms as i64,
                    record.started_at_ms.map(|value| value as i64),
                    record.ended_at_ms.map(|value| value as i64),
                    record.summary.last_error,
                ],
            )
            .map_err(|error| format!("Failed to save record `{}`: {error}", record.summary.id))?;
        Ok(())
    }

    pub fn load_record(&self, id: &str) -> Result<Option<StoredRecord>, String> {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT id, name, config_json, state, created_at_ms, updated_at_ms,
                        started_at_ms, ended_at_ms, last_error
                 FROM records WHERE id = ?1",
                params![id],
                stored_record_from_row,
            )
            .optional()
            .map_err(|error| format!("Failed to load record `{id}`: {error}"))
    }

    pub fn list_records(&self) -> Result<Vec<StoredRecord>, String> {
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, name, config_json, state, created_at_ms, updated_at_ms,
                        started_at_ms, ended_at_ms, last_error
                 FROM records
                 ORDER BY created_at_ms ASC",
            )
            .map_err(|error| format!("Failed to prepare the record list: {error}"))?;
        let rows = statement
            .query_map([], stored_record_from_row)
            .map_err(|error| format!("Failed to read the record list: {error}"))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|error| format!("Failed to read a record row: {error}"))?);
        }
        Ok(records)
    }

    pub fn insert_source_run(&self, source: &RecordSource) -> Result<(), String> {
        let connection = self.connection();
        connection
            .execute(
                "INSERT INTO source_runs (
                    id, record_id, device_id, device_name, elapsed_base_ms, elapsed_ms,
                    started_at_ms, ended_at_ms, state, error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    source.source_run_id,
                    source.record_id,
                    source.device_id,
                    source.device_name,
                    source.elapsed_base_ms as i64,
                    source.elapsed_ms as i64,
                    source.started_at_ms as i64,
                    source.ended_at_ms.map(|value| value as i64),
                    source.state.as_str(),
                    source.error.as_deref(),
                ],
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "Failed to save source run `{}`: {error}",
                    source.source_run_id
                )
            })
    }

    pub fn update_source_state(
        &self,
        source_run_id: &str,
        state: SourceRunState,
        error: Option<&str>,
    ) -> Result<(), String> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE source_runs SET state = ?1, error = ?2 WHERE id = ?3",
                params![state.as_str(), error, source_run_id],
            )
            .map_err(|error| format!("Failed to update source run `{source_run_id}`: {error}"))?;
        Ok(())
    }

    pub fn finish_source_run(
        &self,
        source_run_id: &str,
        state: SourceRunState,
        ended_at_ms: u64,
        elapsed_ms: u64,
        error: Option<&str>,
    ) -> Result<(), String> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE source_runs
                 SET state = ?1, ended_at_ms = ?2, elapsed_ms = MAX(elapsed_ms, ?3), error = ?4
                 WHERE id = ?5",
                params![
                    state.as_str(),
                    ended_at_ms as i64,
                    elapsed_ms as i64,
                    error,
                    source_run_id
                ],
            )
            .map_err(|error| format!("Failed to finish source run `{source_run_id}`: {error}"))?;
        Ok(())
    }

    pub fn touch_source_elapsed(&self, source_run_id: &str, elapsed_ms: u64) -> Result<(), String> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE source_runs SET elapsed_ms = MAX(elapsed_ms, ?1) WHERE id = ?2",
                params![elapsed_ms as i64, source_run_id],
            )
            .map_err(|error| {
                format!("Failed to update source elapsed time `{source_run_id}`: {error}")
            })?;
        Ok(())
    }

    pub fn device_elapsed_ms(&self, record_id: &str, device_id: &str) -> Result<u64, String> {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT COALESCE(MAX(elapsed_base_ms + elapsed_ms), 0)
                 FROM source_runs WHERE record_id = ?1 AND device_id = ?2",
                params![record_id, device_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value.max(0) as u64)
            .map_err(|error| {
                format!("Failed to read elapsed device time for `{device_id}`: {error}")
            })
    }

    pub fn list_source_runs(&self, record_id: &str) -> Result<Vec<RecordSource>, String> {
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, device_id, device_name, state, elapsed_base_ms, elapsed_ms,
                        started_at_ms, ended_at_ms, error
                 FROM source_runs
                 WHERE record_id = ?1
                 ORDER BY started_at_ms ASC, id ASC",
            )
            .map_err(|error| format!("Failed to prepare source run list: {error}"))?;
        let rows = statement
            .query_map(params![record_id], |row| {
                let state: String = row.get(3)?;
                Ok(RecordSource {
                    source_run_id: row.get(0)?,
                    record_id: record_id.to_owned(),
                    device_id: row.get(1)?,
                    device_name: row.get(2)?,
                    state: SourceRunState::parse(&state),
                    elapsed_base_ms: get_u64(row, 4)?,
                    elapsed_ms: get_u64(row, 5)?,
                    started_at_ms: get_u64(row, 6)?,
                    ended_at_ms: get_optional_u64(row, 7)?,
                    error: row.get(8)?,
                })
            })
            .map_err(|error| format!("Failed to read source run list: {error}"))?;
        let mut sources = Vec::new();
        for row in rows {
            sources.push(row.map_err(|error| format!("Failed to read a source run: {error}"))?);
        }
        Ok(sources)
    }

    pub fn upsert_subtitle(&self, segment: &SubtitleSegment) -> Result<(), String> {
        let speaker_json = serialized_optional(&segment.speaker)?;
        let translation_json = serialized_optional(&segment.translation)?;
        let connection = self.connection();
        connection
            .execute(
                "INSERT INTO subtitles (
                    id, record_id, source_run_id, device_id, device_name, local_id,
                    device_time_secs, start_secs, end_secs, occurrence_ms, text, revision,
                    is_final, speaker_json, speaker_revision, translation_json,
                    source_language, created_at_ms, updated_at_ms
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16, ?17, ?18, ?19
                 )
                 ON CONFLICT(source_run_id, local_id) DO UPDATE SET
                    device_name = excluded.device_name,
                    device_time_secs = excluded.device_time_secs,
                    start_secs = excluded.start_secs,
                    end_secs = excluded.end_secs,
                    occurrence_ms = excluded.occurrence_ms,
                    text = excluded.text,
                    revision = excluded.revision,
                    is_final = excluded.is_final,
                    speaker_json = excluded.speaker_json,
                    speaker_revision = excluded.speaker_revision,
                    translation_json = excluded.translation_json,
                    source_language = excluded.source_language,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    segment.id,
                    segment.record_id,
                    segment.source_run_id,
                    segment.device_id,
                    segment.device_name,
                    segment.local_id as i64,
                    segment.device_time_secs,
                    segment.start_secs,
                    segment.end_secs,
                    segment.occurrence_ms as i64,
                    segment.text,
                    segment.revision,
                    segment.is_final,
                    speaker_json,
                    segment.speaker_revision,
                    translation_json,
                    segment.source_language,
                    segment.created_at_ms as i64,
                    segment.updated_at_ms as i64,
                ],
            )
            .map_err(|error| format!("Failed to save subtitle `{}`: {error}", segment.id))?;
        Ok(())
    }

    pub fn list_recent_segments(
        &self,
        record_id: &str,
        limit: usize,
    ) -> Result<Vec<SubtitleSegment>, String> {
        let limit = limit.clamp(1, 2_000);
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, session_id, record_id, source_run_id, device_id, device_name,
                        local_id, device_time_secs, start_secs, end_secs, occurrence_ms,
                        text, revision, is_final, speaker_json, speaker_revision,
                        translation_json, source_language, created_at_ms, updated_at_ms
                 FROM (
                    SELECT id, source_run_id AS session_id, record_id, source_run_id,
                           device_id, device_name, local_id, device_time_secs, start_secs,
                           end_secs, occurrence_ms, text, revision, is_final, speaker_json,
                           speaker_revision, translation_json, source_language,
                           created_at_ms, updated_at_ms
                    FROM subtitles
                    WHERE record_id = ?1
                    ORDER BY occurrence_ms DESC, id DESC
                    LIMIT ?2
                 )
                 ORDER BY occurrence_ms ASC, id ASC",
            )
            .map_err(|error| format!("Failed to prepare recent subtitles: {error}"))?;
        let rows = statement
            .query_map(params![record_id, limit as i64], segment_from_row)
            .map_err(|error| format!("Failed to read recent subtitles: {error}"))?;
        let mut segments = Vec::new();
        for row in rows {
            segments.push(row.map_err(|error| format!("Failed to read a subtitle: {error}"))?);
        }
        Ok(segments)
    }

    pub fn list_subtitles(
        &self,
        record_id: &str,
        before: Option<&SubtitleCursor>,
        limit: usize,
    ) -> Result<RecordSubtitlePage, String> {
        let limit = limit.clamp(1, 500);
        let request_limit = limit.saturating_add(1);
        let connection = self.connection();
        let select = "SELECT id, source_run_id AS session_id, record_id, source_run_id,
                             device_id, device_name, local_id, device_time_secs,
                             start_secs, end_secs, occurrence_ms, text, revision,
                             is_final, speaker_json, speaker_revision, translation_json,
                             source_language, created_at_ms, updated_at_ms
                      FROM subtitles";
        let mut segments = if let Some(cursor) = before {
            let sql = format!(
                "{select}
                 WHERE record_id = ?1
                   AND (occurrence_ms < ?2 OR (occurrence_ms = ?2 AND id < ?3))
                 ORDER BY occurrence_ms DESC, id DESC
                 LIMIT ?4"
            );
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("Failed to prepare subtitle history: {error}"))?;
            let rows = statement
                .query_map(
                    params![
                        record_id,
                        cursor.occurrence_ms as i64,
                        cursor.id,
                        request_limit as i64
                    ],
                    segment_from_row,
                )
                .map_err(|error| format!("Failed to read subtitle history: {error}"))?;
            collect_segments(rows)?
        } else {
            let sql = format!(
                "{select}
                 WHERE record_id = ?1
                 ORDER BY occurrence_ms DESC, id DESC
                 LIMIT ?2"
            );
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("Failed to prepare subtitle history: {error}"))?;
            let rows = statement
                .query_map(params![record_id, request_limit as i64], segment_from_row)
                .map_err(|error| format!("Failed to read subtitle history: {error}"))?;
            collect_segments(rows)?
        };
        let has_more = segments.len() > limit;
        if has_more {
            segments.truncate(limit);
        }
        segments.reverse();
        let next_before = if has_more {
            segments.first().map(|segment| SubtitleCursor {
                occurrence_ms: segment.occurrence_ms,
                id: segment.id.clone(),
            })
        } else {
            None
        };
        Ok(RecordSubtitlePage {
            items: segments,
            has_more,
            next_before,
        })
    }

    pub fn subtitle_count(&self, record_id: &str) -> Result<u64, String> {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT COUNT(*) FROM subtitles WHERE record_id = ?1",
                params![record_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value.max(0) as u64)
            .map_err(|error| format!("Failed to count subtitles: {error}"))
    }

    fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn stored_record_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRecord> {
    let config_json: String = row.get(2)?;
    let config = serde_json::from_str::<AppConfig>(&config_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            config_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let state: String = row.get(3)?;
    Ok(StoredRecord {
        summary: super::types::RecordSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            state: RecordState::parse(&state),
            device_ids: config.audio.device_ids.clone(),
            active_device_ids: Vec::new(),
            has_active_source: false,
            last_error: row.get(8)?,
            created_at_ms: get_u64(row, 4)?,
            updated_at_ms: get_u64(row, 5)?,
            revision: 0,
        },
        config,
        started_at_ms: get_optional_u64(row, 6)?,
        ended_at_ms: get_optional_u64(row, 7)?,
    })
}

fn segment_from_row(row: &Row<'_>) -> rusqlite::Result<SubtitleSegment> {
    let speaker_json: Option<String> = row.get(14)?;
    let translation_json: Option<String> = row.get(16)?;
    let speaker = deserialize_optional::<SpeakerInfo>(speaker_json)?;
    let translation = deserialize_optional::<TranslationState>(translation_json)?;
    Ok(SubtitleSegment {
        id: row.get(0)?,
        session_id: row.get(1)?,
        record_id: row.get(2)?,
        source_run_id: row.get(3)?,
        device_id: row.get(4)?,
        device_name: row.get(5)?,
        local_id: get_u64(row, 6)?,
        device_time_secs: row.get(7)?,
        start_secs: row.get(8)?,
        end_secs: row.get(9)?,
        occurrence_ms: get_u64(row, 10)?,
        text: row.get(11)?,
        revision: row.get(12)?,
        is_final: row.get(13)?,
        speaker,
        speaker_revision: row.get(15)?,
        translation,
        source_language: row.get(17)?,
        created_at_ms: get_u64(row, 18)?,
        updated_at_ms: get_u64(row, 19)?,
        last_change: Instant::now(),
    })
}

fn get_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value.max(0) as u64)
}

fn get_optional_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value.max(0) as u64))
}

fn serialized_optional<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>, String> {
    match value {
        Some(value) => serde_json::to_string(value)
            .map(Some)
            .map_err(|error| format!("Failed to serialize subtitle metadata: {error}")),
        None => Ok(None),
    }
}

fn deserialize_optional<T: serde::de::DeserializeOwned>(
    value: Option<String>,
) -> rusqlite::Result<Option<T>> {
    value
        .map(|value| {
            serde_json::from_str::<T>(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    value.len(),
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

fn collect_segments(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<SubtitleSegment>>,
) -> Result<Vec<SubtitleSegment>, String> {
    let mut segments = Vec::new();
    for row in rows {
        segments.push(row.map_err(|error| format!("Failed to read a subtitle: {error}"))?);
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::TranslationStatus;

    fn config() -> AppConfig {
        let mut config = AppConfig::default();
        config.audio.device_ids = vec!["device-a".to_owned()];
        config
    }

    fn segment_for(
        source_run_id: &str,
        device_id: &str,
        local_id: u64,
        occurrence_ms: u64,
        text: &str,
    ) -> SubtitleSegment {
        SubtitleSegment {
            id: format!("{source_run_id}:{local_id}"),
            session_id: source_run_id.to_owned(),
            record_id: "record".to_owned(),
            source_run_id: source_run_id.to_owned(),
            device_id: device_id.to_owned(),
            device_name: device_id.to_owned(),
            local_id,
            start_secs: occurrence_ms as f64 / 1_000.0,
            end_secs: Some(occurrence_ms as f64 / 1_000.0 + 1.0),
            device_time_secs: occurrence_ms as f64 / 1_000.0,
            occurrence_ms,
            text: text.to_owned(),
            revision: 1,
            is_final: true,
            speaker: None,
            speaker_revision: 0,
            translation: Some(TranslationState {
                target_language: "zh-Hans".to_owned(),
                text: Some("你好".to_owned()),
                status: TranslationStatus::Ready,
                revision: 1,
                engine: "bergamot".to_owned(),
                mode: "text".to_owned(),
                model_id: "bergamot-en-zh-Hans".to_owned(),
                error: None,
                updated_at_ms: 10,
            }),
            source_language: Some("en".to_owned()),
            created_at_ms: occurrence_ms,
            updated_at_ms: occurrence_ms,
            last_change: Instant::now(),
        }
    }

    fn segment() -> SubtitleSegment {
        segment_for("run-a", "device-a", 1, 10_000, "hello")
    }

    #[test]
    fn records_sources_and_subtitles_survive_a_reopen() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
        let path = directory.join("records.sqlite");
        let store = RecordStore::open(path.clone()).unwrap();
        let record = StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                device_ids: vec!["device-a".to_owned()],
                active_device_ids: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: None,
        };
        store.save_record(&record).unwrap();
        store
            .insert_source_run(&RecordSource {
                source_run_id: "run-a".to_owned(),
                record_id: "record".to_owned(),
                device_id: "device-a".to_owned(),
                device_name: "Device A".to_owned(),
                state: SourceRunState::Running,
                elapsed_base_ms: 0,
                elapsed_ms: 2_000,
                started_at_ms: 2,
                ended_at_ms: None,
                error: None,
            })
            .unwrap();
        store.upsert_subtitle(&segment()).unwrap();
        drop(store);

        let store = RecordStore::open(path).unwrap();
        assert_eq!(store.list_records().unwrap().len(), 1);
        assert_eq!(
            store.device_elapsed_ms("record", "device-a").unwrap(),
            2_000
        );
        let page = store.list_subtitles("record", None, 50).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].translation.as_ref().unwrap().text.as_deref(),
            Some("你好")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn identical_local_ids_from_different_source_runs_coexist() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
        let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
        store
            .save_record(&StoredRecord {
                summary: super::super::types::RecordSummary {
                    id: "record".to_owned(),
                    name: "Record".to_owned(),
                    state: RecordState::Paused,
                    device_ids: vec!["device-a".to_owned(), "device-b".to_owned()],
                    active_device_ids: Vec::new(),
                    has_active_source: false,
                    last_error: None,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    revision: 0,
                },
                config: config(),
                started_at_ms: Some(1),
                ended_at_ms: None,
            })
            .unwrap();
        store
            .upsert_subtitle(&segment_for("run-a", "device-a", 1, 1_000, "from A"))
            .unwrap();
        store
            .upsert_subtitle(&segment_for("run-b", "device-b", 1, 2_000, "from B"))
            .unwrap();

        let page = store.list_subtitles("record", None, 50).unwrap();
        let texts = page
            .items
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["from A", "from B"]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn complete_history_can_be_paged_after_the_live_window_is_trimmed() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
        let path = directory.join("records.sqlite");
        let store = RecordStore::open(path.clone()).unwrap();
        store
            .save_record(&StoredRecord {
                summary: super::super::types::RecordSummary {
                    id: "record".to_owned(),
                    name: "Record".to_owned(),
                    state: RecordState::Ended,
                    device_ids: vec!["device-a".to_owned()],
                    active_device_ids: Vec::new(),
                    has_active_source: false,
                    last_error: None,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    revision: 0,
                },
                config: config(),
                started_at_ms: Some(1),
                ended_at_ms: Some(2),
            })
            .unwrap();
        for local_id in 0..7 {
            store
                .upsert_subtitle(&segment_for(
                    "run-a",
                    "device-a",
                    local_id,
                    local_id * 1_000,
                    &format!("caption {local_id}"),
                ))
                .unwrap();
        }
        assert_eq!(store.list_recent_segments("record", 3).unwrap().len(), 3);
        drop(store);

        let store = RecordStore::open(path).unwrap();
        let mut before = None;
        let mut pages = Vec::new();
        loop {
            let page = store.list_subtitles("record", before.as_ref(), 2).unwrap();
            pages.push(
                page.items
                    .iter()
                    .map(|segment| segment.local_id)
                    .collect::<Vec<_>>(),
            );
            if !page.has_more {
                break;
            }
            before = page.next_before;
        }
        pages.reverse();
        let ids = pages.into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(ids, (0..7).collect::<Vec<_>>());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn interrupted_records_and_sources_recover_as_paused_and_disconnected() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
        let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
        store
            .save_record(&StoredRecord {
                summary: super::super::types::RecordSummary {
                    id: "record".to_owned(),
                    name: "Record".to_owned(),
                    state: RecordState::Running,
                    device_ids: vec!["device-a".to_owned()],
                    active_device_ids: vec!["device-a".to_owned()],
                    has_active_source: true,
                    last_error: None,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    revision: 0,
                },
                config: config(),
                started_at_ms: Some(1),
                ended_at_ms: None,
            })
            .unwrap();
        store
            .insert_source_run(&RecordSource {
                source_run_id: "run-a".to_owned(),
                record_id: "record".to_owned(),
                device_id: "device-a".to_owned(),
                device_name: "Device A".to_owned(),
                state: SourceRunState::Running,
                elapsed_base_ms: 0,
                elapsed_ms: 2_000,
                started_at_ms: 2,
                ended_at_ms: None,
                error: None,
            })
            .unwrap();

        store.recover_active_records(123).unwrap();

        let record = store.load_record("record").unwrap().unwrap();
        assert_eq!(record.summary.state, RecordState::Paused);
        assert_eq!(record.summary.updated_at_ms, 123);
        let sources = store.list_source_runs("record").unwrap();
        assert_eq!(sources[0].state, SourceRunState::Disconnected);
        assert_eq!(sources[0].ended_at_ms, Some(123));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
