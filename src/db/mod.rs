use rusqlite::Connection;

pub fn init_db() -> anyhow::Result<Connection> {
    let conn = Connection::open("terrek.db")?;

    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;

        CREATE TABLE IF NOT EXISTS commands (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            command TEXT,
            output TEXT,
            timestamp INTEGER
        );
        ",
    )?;

    Ok(conn)
}

pub fn store_command(
    conn: &Connection,
    session_id: &str,
    command: &str,
    output: &str,
    timestamp: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO commands (session_id, command, output, timestamp)
         VALUES (?1, ?2, ?3, ?4)",
        (session_id, command, output, timestamp),
    )?;
    Ok(())
}

pub mod worker;
