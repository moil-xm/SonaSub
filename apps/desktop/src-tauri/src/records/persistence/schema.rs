use rusqlite::Connection;

pub(super) fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
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
}
