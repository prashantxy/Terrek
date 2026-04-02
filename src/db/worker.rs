use std::sync::mpsc::{Sender, channel};
use std::thread;

use rusqlite::{Connection, params};

#[derive(Debug)]
pub enum DbEvent {
    StoreCommand {
        session_id: String,
        command: String,
        output: String,
        timestamp: i64,
    },
}

pub fn start_db_worker() -> Sender<DbEvent> {
    let (tx, rx) = channel::<DbEvent>();

    thread::spawn(move || {
        let conn = Connection::open("terrek.db").expect("Failed to open DB");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS commands (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                command TEXT,
                output TEXT,
                timestamp INTEGER
            )",
            [],
        )
        .expect("Failed to create table");

        while let Ok(event) = rx.recv() {
            match event {
                DbEvent::StoreCommand {
                    session_id,
                    command,
                    output,
                    timestamp,
                } => {
                    let _ = conn.execute(
                        "INSERT INTO commands (session_id, command, output, timestamp)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![session_id, command, output, timestamp],
                    );
                }
            }
        }
    });

    tx
}
