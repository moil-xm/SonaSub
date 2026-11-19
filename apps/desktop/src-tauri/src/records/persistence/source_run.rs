use rusqlite::{params, Connection};

use super::{
    super::types::{RecordSource, SourceRunState},
    get_optional_u64, get_u64,
    store::RecordStore,
};

impl RecordStore {
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
        touch_source_elapsed_on(&connection, source_run_id, elapsed_ms)
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
}

pub(super) fn touch_source_elapsed_on(
    connection: &Connection,
    source_run_id: &str,
    elapsed_ms: u64,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE source_runs SET elapsed_ms = MAX(elapsed_ms, ?1) WHERE id = ?2",
            params![elapsed_ms as i64, source_run_id],
        )
        .map(|_| ())
        .map_err(|error| format!("Failed to update source elapsed time `{source_run_id}`: {error}"))
}
