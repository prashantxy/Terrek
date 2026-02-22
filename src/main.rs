use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{unbounded, Sender as CbSender, Receiver as CbReceiver};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use uuid::Uuid;

mod ai;
mod db;
mod commands;
mod context;
mod config;

use db::worker::{start_db_worker, DbEvent};
use commands::{handle_command, TerrekAction};
use context::state::ContextState;

enum Mode {
    Shell,
    Terrek,
}


struct SuggestionEngine {
    static_commands: Vec<String>,
    history: Vec<String>,
    ai_cache: Vec<String>,
}

impl SuggestionEngine {
    fn suggest(&self, input: &str) -> Vec<String> {
        if input.is_empty() {
            return vec![];
        }

        let matcher = SkimMatcherV2::default();
        let mut scored = Vec::new();

        let sources = self
            .static_commands
            .iter()
            .chain(self.history.iter())
            .chain(self.ai_cache.iter());

        for item in sources {
            if let Some(score) = matcher.fuzzy_match(item, input) {
                scored.push((score, item.clone()));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, s)| s).take(5).collect()
    }
}

fn start_ai_worker(
    rx: CbReceiver<String>,
    tx: CbSender<Vec<String>>,
    context: Arc<Mutex<ContextState>>,
) {
    thread::spawn(move || {
        let mut last_input = String::new();

        for input in rx {
            if input.len() < 3 || input == last_input {
                continue;
            }

            last_input = input.clone();

            thread::sleep(std::time::Duration::from_millis(400));

            let prompt = format!(
                "You are a CLI suggestion engine.
User typed: \"{}\"
Suggest 5 possible Terrek commands.
Return only command list.",
                input
            );

            let ctx = context.lock().unwrap();

            match ai::gemini::ask_gemini(&ctx, &prompt) {
                Ok(resp) => {
                    let suggestions = resp
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>();

                    tx.send(suggestions).ok();
                }
                Err(_) => {}
            }
        }
    });
}

fn draw_prompt(buf: &str, suggestion: Option<&String>) {
    print!("\r\x1B[K[Terrek] > {}", buf);

    if let Some(s) = suggestion {
        if s.starts_with(buf) {
            let ghost = &s[buf.len()..];
            print!("\x1B[90m{}\x1B[0m", ghost);
        }
    }

    std::io::stdout().flush().ok();
}

fn clear_prompt() {
    print!("\r\x1B[K");
    std::io::stdout().flush().ok();
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let db_tx: Sender<DbEvent> = start_db_worker();
    let session_id = Uuid::new_v4().to_string();

    let context = Arc::new(Mutex::new(ContextState::new()));

    let (ai_tx, ai_rx) = unbounded();
    let (ai_out_tx, ai_out_rx) = unbounded();

    let engine = Arc::new(Mutex::new(SuggestionEngine {
        static_commands: vec![
            "terrek hello".into(),
            "terrek time".into(),
            "terrek history".into(),
            "terrek ai".into(),
            "terrek exit".into(),
        ],
        history: vec![],
        ai_cache: vec![],
    }));

    start_ai_worker(ai_rx, ai_out_tx, context.clone());

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());
    let cmd = CommandBuilder::new(shell);
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, out_rx) = channel::<String>();

    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buffer[..n]).to_string();
                    out_tx.send(text).ok();
                }
                Err(_) => break,
            }
        }
    });

    let mut mode = Mode::Shell;
    let mut shell_input_buffer = String::new();
    let mut terrek_buffer = String::new();
    let mut current_output = String::new();

    println!("You are in SHELL session. Press Ctrl+T for TERREK mode.");

    loop {

        if let Ok(ai_suggestions) = ai_out_rx.try_recv() {
            let mut eng = engine.lock().unwrap();
            eng.ai_cache = ai_suggestions;
        }

        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();
            current_output.push_str(&text);
        }

        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {

                KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Terrek;
                    terrek_buffer.clear();
                    println!("\n-- TERREK MODE --");
                    draw_prompt("", None);
                }

                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        shell_input_buffer.push(c);
                        writer.write_all(&[c as u8])?;
                        writer.flush()?;
                    }
                    Mode::Terrek => {
                        terrek_buffer.push(c);
                        ai_tx.send(terrek_buffer.clone()).ok();

                        let suggestions = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer)
                        };

                        draw_prompt(&terrek_buffer, suggestions.first());
                    }
                },

                KeyCode::Right => {
                    if let Mode::Terrek = mode {
                        let suggestion = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer).first().cloned()
                        };

                        if let Some(s) = suggestion {
                            terrek_buffer = s;
                            draw_prompt(&terrek_buffer, None);
                        }
                    }
                }

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        shell_input_buffer.pop();
                        writer.write_all(b"\x7f")?;
                        writer.flush()?;
                    }
                    Mode::Terrek => {
                        terrek_buffer.pop();

                        let suggestions = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer)
                        };

                        draw_prompt(&terrek_buffer, suggestions.first());
                    }
                },

                KeyCode::Enter => match mode {

                    Mode::Shell => {
                        current_output.clear();

                        let full_command =
                            format!("{}; echo __TERREK_EXIT__$?\n", shell_input_buffer);

                        writer.write_all(full_command.as_bytes())?;
                        writer.flush()?;

                        {
                            let mut ctx = context.lock().unwrap();
                            ctx.add_command(shell_input_buffer.clone());
                        }

                        shell_input_buffer.clear();
                    }

                    Mode::Terrek => {
                        println!();
                        let input = terrek_buffer.trim();

                        {
                            let mut eng = engine.lock().unwrap();
                            eng.history.push(input.to_string());
                        }

                        if input.starts_with("terrek ") {
                            let stripped =
                                input.trim_start_matches("terrek ").trim();

                            let ctx = context.lock().unwrap();
                            let action =
                                handle_command(&ctx, stripped)?;

                            match action {
                                TerrekAction::Output(text) => {
                                    println!("[Terrek] {}", text);
                                }
                            }
                         } else {
                            writer.write_all(format!("{}\n", input).as_bytes())?;
                            writer.flush()?;
                         }

                        terrek_buffer.clear();
                        draw_prompt("", None);
                    }
                },

                KeyCode::Esc => match mode {
                     Mode::Terrek => {
                        clear_prompt();
                        mode = Mode::Shell;
                        println!("\n-- SHELL MODE --");
                    }
                    Mode::Shell => break Ok(()),
                },
 
                _ => {}
            }
        }
    }
}
