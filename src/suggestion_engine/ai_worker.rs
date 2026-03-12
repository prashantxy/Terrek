use crossbeam_channel::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use crate::ai::gemini::ask_gemini;
use crate::context::ContextState;


pub fn start_ai_worker(
    rx: Receiver<String>,
    tx: Sender<Vec<String>>,
    context: ContextState,
) {
    thread::spawn(move || {
        for input in rx {

            if input.len() < 3 {
                continue;
            }

            // debounce
            thread::sleep(Duration::from_millis(400));

            let prompt = format!(
                "You are a CLI suggestion engine.
User typed: \"{}\"
Suggest 5 possible Terrek commands.
Return only command list, one per line.
No explanation.",
                input
            );

            match ask_gemini(&context, &prompt) {
                Ok(response) => {
                    let suggestions: Vec<String> =
                        response.lines().map(|l| l.trim().to_string()).collect();

                    let _ = tx.send(suggestions);
                }
                Err(_) => {}
            }
        }
    });
}
