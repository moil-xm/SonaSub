use std::time::Instant;

use rusqlite::{params, Connection, Row};

use crate::{
    audio::AudioSourceKind,
    speaker::SpeakerInfo,
    subtitles::{SubtitleSegment, TranslationState},
};

use super::{
    super::types::{RecordSubtitlePage, SubtitleCursor},
    get_u64,
    source_run::touch_source_elapsed_on,
    store::RecordStore,
};

impl RecordStore {
    pub fn upsert_subtitle(&self, segment: &SubtitleSegment) -> Result<(), String> {
        self.upsert_subtitles(std::slice::from_ref(segment), None, 0)
    }

    pub fn upsert_subtitles(
        &self,
        segments: &[SubtitleSegment],
        source_run_id: Option<&str>,
        elapsed_ms: u64,
    ) -> Result<(), String> {
        if segments.is_empty() && elapsed_ms == 0 {
            return Ok(());
        }
        let mut connection = self.connection();
        let transaction = connection
            .transaction()
            .map_err(|error| format!("Failed to start the subtitle transaction: {error}"))?;
        for segment in segments {
            upsert_subtitle_on(&transaction, segment)?;
        }
        if elapsed_ms > 0 {
            let source_run_id = source_run_id.ok_or_else(|| {
                "A subtitle batch with elapsed time must identify its source run.".to_owned()
            })?;
            touch_source_elapsed_on(&transaction, source_run_id, elapsed_ms)?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Failed to commit the subtitle transaction: {error}"))
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
                "SELECT id, session_id, record_id, source_run_id, source_kind, device_id, device_name,
                        local_id, device_time_secs, start_secs, end_secs, occurrence_ms,
                        text, revision, is_final, speaker_json, speaker_revision,
                        translation_json, source_language, created_at_ms, updated_at_ms
                 FROM (
                    SELECT id, source_run_id AS session_id, record_id, source_run_id,
                           source_kind, device_id, device_name, local_id, device_time_secs, start_secs,
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
                             source_kind, device_id, device_name, local_id, device_time_secs,
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
}

fn upsert_subtitle_on(connection: &Connection, segment: &SubtitleSegment) -> Result<(), String> {
    let speaker_json = serialized_optional(&segment.speaker)?;
    let translation_json = serialized_optional(&segment.translation)?;
    connection
        .execute(
            "INSERT INTO subtitles (
                    id, record_id, source_run_id, source_kind, device_id, device_name, local_id,
                    device_time_secs, start_secs, end_secs, occurrence_ms, text, revision,
                    is_final, speaker_json, speaker_revision, translation_json,
                    source_language, created_at_ms, updated_at_ms
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16, ?17, ?18, ?19, ?20
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
                segment.source_kind.as_str(),
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

fn segment_from_row(row: &Row<'_>) -> rusqlite::Result<SubtitleSegment> {
    let source_kind: String = row.get(4)?;
    let speaker_json: Option<String> = row.get(15)?;
    let translation_json: Option<String> = row.get(17)?;
    let speaker = deserialize_optional::<SpeakerInfo>(speaker_json)?;
    let translation = deserialize_optional::<TranslationState>(translation_json)?;
    Ok(SubtitleSegment {
        id: row.get(0)?,
        session_id: row.get(1)?,
        record_id: row.get(2)?,
        source_run_id: row.get(3)?,
        source_kind: AudioSourceKind::parse(&source_kind).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                source_kind.len(),
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        device_id: row.get(5)?,
        device_name: row.get(6)?,
        local_id: get_u64(row, 7)?,
        device_time_secs: row.get(8)?,
        start_secs: row.get(9)?,
        end_secs: row.get(10)?,
        occurrence_ms: get_u64(row, 11)?,
        text: row.get(12)?,
        revision: row.get(13)?,
        is_final: row.get(14)?,
        speaker,
        speaker_revision: row.get(16)?,
        translation,
        source_language: row.get(18)?,
        created_at_ms: get_u64(row, 19)?,
        updated_at_ms: get_u64(row, 20)?,
        last_change: Instant::now(),
    })
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
