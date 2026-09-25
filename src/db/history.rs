//! Command history in SQLite (`<data dir>/terrek/history.db`).

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::{Path, PathBuf};

use crate::config::data_dir;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandRecord {
    pub id: Option<i64>,
    pub session_id: Option<String>,
    pub command: String,
    /// ANSI-stripped tail of what the command printed.
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub cwd: Option<String>,
    pub duration_ms: Option<i64>,
    /// Unix seconds when the command finished.
    pub timestamp: i64,
}

impl CommandRecord {
    pub fn failed(&self) -> bool {
        self.exit_code.is_some_and(|c| c != 0)
    }
}

pub struct HistoryStore {
    conn: Connection,
}

const SCHEMA_VERSION: i32 = 1;

pub fn default_path() -> PathBuf {
    data_dir().join("history.db")
}

impl HistoryStore {
    pub fn open_default() -> Result<Self> {
        Self::open(&default_path())
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        restrict_permissions(path);
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS commands (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id  TEXT,
                    command     TEXT NOT NULL,
                    output      TEXT,
                    exit_code   INTEGER,
                    cwd         TEXT,
                    duration_ms INTEGER,
                    timestamp   INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS commands_timestamp ON commands(timestamp);
                PRAGMA user_version = 1;",
            )?;
        }
        Ok(Self { conn })
    }

    pub fn insert(&self, record: &CommandRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO commands (session_id, command, output, exit_code, cwd, duration_ms, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.session_id,
                record.command,
                record.output,
                record.exit_code,
                record.cwd,
                record.duration_ms,
                record.timestamp
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Newest first.
    pub fn recent(&self, limit: usize, failed_only: bool) -> Result<Vec<CommandRecord>> {
        let sql = if failed_only {
            "SELECT * FROM commands WHERE exit_code IS NOT NULL AND exit_code != 0
             ORDER BY id DESC LIMIT ?1"
        } else {
            "SELECT * FROM commands ORDER BY id DESC LIMIT ?1"
        };
        self.query(sql, params![limit as i64])
    }

    /// Case-insensitive substring match on the command text, newest first.
    pub fn search(&self, needle: &str, limit: usize) -> Result<Vec<CommandRecord>> {
        let escaped = needle
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        self.query(
            "SELECT * FROM commands WHERE command LIKE ?1 ESCAPE '\\' ORDER BY id DESC LIMIT ?2",
            params![format!("%{escaped}%"), limit as i64],
        )
    }

    pub fn last_failed(&self) -> Result<Option<CommandRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT * FROM commands WHERE exit_code IS NOT NULL AND exit_code != 0
                 ORDER BY id DESC LIMIT 1",
                [],
                from_row,
            )
            .optional()?)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))?)
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn from_row(row: &Row) -> rusqlite::Result<CommandRecord> {
    Ok(CommandRecord {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        command: row.get("command")?,
        output: row.get("output")?,
        exit_code: row.get("exit_code")?,
        cwd: row.get("cwd")?,
        duration_ms: row.get("duration_ms")?,
        timestamp: row.get("timestamp")?,
    })
}

/// History can contain secrets that were typed or printed; keep it owner-only.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(command: &str, exit: i32, ts: i64) -> CommandRecord {
        CommandRecord {
            command: command.into(),
            exit_code: Some(exit),
            timestamp: ts,
            ..Default::default()
        }
    }

    #[test]
    fn insert_and_read_back_newest_first() {
        let store = HistoryStore::open_in_memory().unwrap();
        store.insert(&record("ls", 0, 1)).unwrap();
        store.insert(&record("cargo build", 101, 2)).unwrap();
        let recent = store.recent(10, false).unwrap();
        assert_eq!(recent[0].command, "cargo build");
        assert_eq!(recent[1].command, "ls");
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn failed_filter_and_last_failed() {
        let store = HistoryStore::open_in_memory().unwrap();
        store.insert(&record("false", 1, 1)).unwrap();
        store.insert(&record("true", 0, 2)).unwrap();
        assert_eq!(store.recent(10, true).unwrap().len(), 1);
        assert_eq!(store.last_failed().unwrap().unwrap().command, "false");
    }

    #[test]
    fn search_treats_wildcards_literally() {
        let store = HistoryStore::open_in_memory().unwrap();
        store.insert(&record("echo 100%", 0, 1)).unwrap();
        store.insert(&record("echo 100", 0, 2)).unwrap();
        let hits = store.search("100%", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].command, "echo 100%");
    }

    #[test]
    fn reopening_a_file_keeps_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.db");
        HistoryStore::open(&path)
            .unwrap()
            .insert(&record("ls", 0, 1))
            .unwrap();
        assert_eq!(HistoryStore::open(&path).unwrap().count().unwrap(), 1);
    }
}
