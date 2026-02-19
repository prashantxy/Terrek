use crossbeam_channel::{Receiver,Sender};
use std::thread;
use std::time::Duration;

pub fn start_ai_worker(
    rx : Receiver<String>;
    tx : Sender<Vec<String>>;
){
    thread::spawn(move ||{
        for input in rx {
            if input.len()<3{
                continue;
            }
        std::thread::sleep(Duration::from_millis(400));

        let suggestion = 
        }
    })
}