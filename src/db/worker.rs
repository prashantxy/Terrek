//! Writes history on its own thread so the interactive loop never waits on disk.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread;

use anyhow::Result;

use super::{CommandRecord, HistoryStore};

pub struct HistoryWriter {
    tx: Sender<CommandRecord>,
}

impl HistoryWriter {
    /// Opens the store up front so a broken database is reported immediately.
    pub fn start(path: PathBuf) -> Result<Self> {
        let store = HistoryStore::open(&path)?;
        let (tx, rx) = channel::<CommandRecord>();
        thread::Builder::new()
            .name("terrek-history".into())
            .spawn(move || {
                for record in rx {
                    // A failed insert must not take down the user's shell.
                    let _ = store.insert(&record);
                }
            })?;
        Ok(Self { tx })
    }

    pub fn record(&self, record: CommandRecord) {
        let _ = self.tx.send(record);
    }
}
