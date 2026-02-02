use std::sync::mpsc::{channel, Sender};
use std::thread;

use crate::db;

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
        let conn = db::init_db().expect("DB init failed");

        for event in rx {
            match event {
                DbEvent::StoreCommand {
                    session_id,
                    command,
                    output,
                    timestamp,
                } => {
                    let _ =
                        db::store_command(&conn, &session_id, &command, &output, timestamp);
                }
            }
        }
    });

    tx
}
