use std::sync::mpsc::{channel,sender}:
use std::thread;

pub enum db_event{
    StoreCommand{
        sessionid: String,
        command: String,
        output: String,
        timestamp: i64,
    },
}