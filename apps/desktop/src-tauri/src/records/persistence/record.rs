use rusqlite::{params, OptionalExtension, Row};

use crate::config::AppConfig;

use super::{
    super::types::{RecordState, SourceRunState, StoredRecord},
    get_optional_u64, get_u64,
    store::RecordStore,
};

impl RecordStore {
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
        let mut config = record.config.clone();
        config.normalize()?;
        let config_json = serde_json::to_string(&config)
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

    pub fn delete_record(&self, record_id: &str) -> Result<bool, String> {
        let mut connection = self.connection();
        let transaction = connection
            .transaction()
            .map_err(|error| format!("Failed to start the record deletion: {error}"))?;
        transaction
            .execute(
                "DELETE FROM subtitles WHERE record_id = ?1",
                params![record_id],
            )
            .map_err(|error| {
                format!("Failed to delete the subtitles of record `{record_id}`: {error}")
            })?;
        transaction
            .execute(
                "DELETE FROM source_runs WHERE record_id = ?1",
                params![record_id],
            )
            .map_err(|error| {
                format!("Failed to delete the source runs of record `{record_id}`: {error}")
            })?;
        let removed = transaction
            .execute("DELETE FROM records WHERE id = ?1", params![record_id])
            .map_err(|error| format!("Failed to delete record `{record_id}`: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Failed to commit the record deletion: {error}"))?;
        Ok(removed > 0)
    }
}

fn stored_record_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRecord> {
    let config_json: String = row.get(2)?;
    let mut config = serde_json::from_str::<AppConfig>(&config_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            config_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    config.normalize().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            config_json.len(),
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    let state: String = row.get(3)?;
    Ok(StoredRecord {
        summary: super::super::types::RecordSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            state: RecordState::parse(&state),
            devices: config.audio.record.clone(),
            active_devices: Vec::new(),
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
