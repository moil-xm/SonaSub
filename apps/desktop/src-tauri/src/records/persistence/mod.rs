mod record;
mod schema;
mod source_run;
mod store;
mod subtitle;

#[cfg(test)]
mod tests;

use rusqlite::Row;

pub use store::RecordStore;

fn get_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value.max(0) as u64)
}

fn get_optional_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value.max(0) as u64))
}
