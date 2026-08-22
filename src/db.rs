//! The event log.
//!
//! One row per detection, whether or not it fired, because the suppressed ones
//! are the interesting record: they tell you how often quiet hours and the
//! cool-down are actually doing something. The schema matches the Python
//! original so an existing database keeps working and the dashboard needs no
//! changes.

use std::path::Path;

use chrono::Local;
use rusqlite::{Connection, Result};

pub struct EventLog {
    conn: Connection,
}

impl EventLog {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                 ts TEXT NOT NULL,
                 fired INTEGER NOT NULL,
                 note TEXT)",
            [],
        )?;
        Ok(Self { conn })
    }

    pub fn log(&self, fired: bool, note: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events(ts, fired, note) VALUES (?1, ?2, ?3)",
            (
                Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                i32::from(fired),
                note,
            ),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EventLog;

    fn scratch(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("tomcat-db-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn creates_schema_and_records_rows() {
        let path = scratch("basic");
        let log = EventLog::open(&path).unwrap();
        log.log(true, "").unwrap();
        log.log(false, "quiet-hours").unwrap();
        log.log(false, "cooldown").unwrap();

        let n: i64 = log
            .conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);

        let fired: i64 = log
            .conn
            .query_row("SELECT count(*) FROM events WHERE fired = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(fired, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reopening_keeps_existing_rows() {
        let path = scratch("reopen");
        EventLog::open(&path).unwrap().log(true, "first").unwrap();
        let log = EventLog::open(&path).unwrap();
        log.log(true, "second").unwrap();
        let n: i64 = log
            .conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2, "reopening must not wipe the log");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn timestamps_are_sortable_iso_8601() {
        let path = scratch("ts");
        let log = EventLog::open(&path).unwrap();
        log.log(true, "").unwrap();
        let ts: String = log
            .conn
            .query_row("SELECT ts FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ts.len(), 19, "expected YYYY-MM-DDTHH:MM:SS, got {ts:?}");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
        let _ = std::fs::remove_file(&path);
    }
}
